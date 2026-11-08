import {expect,test} from '@playwright/test';
import {waitForChat} from './helpers';
test('limit shows saved work and explicit continuation consent',async({page})=>{
 await page.goto('/?scenario=task-paused&theme=dark');await waitForChat(page);
 await page.locator('#alfa-composer').fill('Wykonaj trudne zadanie');await page.locator('#alfa-composer').press('Enter');
 await expect(page.getByText('Postęp zapisany lokalnie',{exact:true})).toBeVisible({timeout:20000});
 await page.getByRole('button',{name:'Przedłuż i wznów',exact:true}).click();
 await expect(page.getByRole('alertdialog',{name:'Zgoda na dalszą pracę?'})).toBeVisible();
 await page.getByRole('button',{name:'Pozostaw wstrzymane',exact:true}).click();
 await expect(page.getByText('Postęp zapisany lokalnie',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Ustawienia przedłużania',exact:true}).click();
 for(const key of ['agents.auto_extend_steps','agents.auto_extend_time','agents.auto_extend_approval']){
  const row=page.locator(`[id="setting-${key}"]`);await expect(row).toBeVisible();
  const toggle=row.getByRole('switch');await expect(toggle).toHaveAttribute('aria-checked','false');await toggle.click();await expect(toggle).toHaveAttribute('aria-checked','true');
 }
});
